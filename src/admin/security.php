<?php 
include("includes/controller.php");
$pagename = 'security';
$container = 'settings';
if(!$session->isSuperAdmin()){
    header("Location: ".$configs->homePage());
    exit;
}
else{
?>
<!DOCTYPE html>
<html>
    <head>
           <title>IPM Software</title>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">

        <link href="css/bootstrap.min.css" rel="stylesheet">
        <link href="fonts/Open Iconic/css/open-iconic-bootstrap.min.css" rel="stylesheet">
        <link href="fonts/font-awesome/css/font-awesome.min.css" rel="stylesheet">

        <link href="css/navigation.css" rel="stylesheet">
        <link href="css/style.css" rel="stylesheet">
        <link href="css/animation.css" rel="stylesheet">
        
    </head>
    <body>
        <!-- Page Wrapper -->
        <div id="page-wrapper">

            <!-- Side Menu -->
            <nav id="side-menu" class="navbar-default navbar-static-side" role="navigation">
                <div id="sidebar-collapse">
                    <div id="logo-element">
                        <a class="logo" href="index.php">
                            <img src="logo2.png">
                        </a>
                    </div>
                    <?php include('navigation.php'); ?>
                </div>
            </nav>
            <!-- END Side Menu -->

            <?php include('top-navbar.php'); ?>      

            <!-- Page Content -->
            <div id="page-content" class="gray-bg">

                <!-- Title Header -->
                <div class="title-header white-bg">
                    <i class="oi oi-star"></i>
                    <h2>Security Settings</h2>
                    <ol class="breadcrumb">
                        <li>
                            <a href="index.php">Home</a>
                        </li>
                        <li class="active">
                            Security Settings
                        </li>
                    </ol>
                </div>
                <!-- END Title Header -->
                
                <div class="row">                                     
                    <div class="col-sm-12 col-md-12">
                        <div class="panel">
                            <div class="panel-body">
                                <h4><strong>Security Settings</strong></h4>
                            </div>
                        </div>
                    </div>                                     
                </div>
             
                <div class="row">                    
                    <div class="col-md-offset-2 col-md-8 col-lg-offset-0 col-lg-6">
                        <div class="panel">
                            <div class="panel-heading">
                                <h2 class="panel-title">Disallow Usernames - Prevent Usernames from being registered</h2>
                            </div>
                            <div class="panel-body">
                                <form class="form-horizontal" id="security-form-validation" role="form" action="includes/adminprocess.php" method="POST"> 
                                    <div class="form-group">
                                        <label for="usernametoban" class="col-sm-3 control-label">Disallow Username <span class="text-danger">*</span></label>
                                        <div class="col-sm-6">
                                            <div class="input-group">
                                                <input class="form-control" name="usernametoban"  id="usernametoban" placeholder="Required Field..">
                                                <span class="input-group-addon"><i class="oi oi-question-mark"></i></span>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="form-group">
                                        <div class="col-sm-offset-3 col-sm-10">
                                            <?php echo $adminfunctions->stopField($session->username, 'configs'); ?>
                                            <input type="hidden" name="form_submission" value="disallow_user">
                                            <button type="submit" class="btn btn-default">Add Username</button>
                                        </div>
                                    </div>
                                </form>
                                <form class="form-horizontal" id="security-form-validation2" role="form" action="includes/adminprocess.php" method="POST">
                                    <div class="form-group">
                                        <label for="disallow_username" class="col-sm-3 control-label">Disallowed Usernames </label>
                                        <div class="col-sm-6">
                                        <select name="username_tounban" id="username_tounban" class="form-control" multiple="multiple">
                                        <?php
                                        $sql = "SELECT ban_username, ban_id FROM banlist WHERE ban_username != ''";
                                        $result = $db->prepare($sql);
                                        $result->execute();
                                        while ($row = $result->fetch()) {
                                            $username = $row['ban_username'];
                                            $ban_id = $row['ban_id'];
                                            echo "<option value='$ban_id'>$username</option>";
                                        }
                                        ?>
                                        </select> 
                                        </div>
                                    </div>
                                    <div class="form-group">
                                        <div class="col-sm-offset-3 col-sm-10">
                                            <?php echo $adminfunctions->stopField($session->username, 'configs'); ?>
                                            <input type="hidden" name="form_submission" value="undisallow_user">
                                            <button type="submit" class="btn btn-default">Remove Disallowed Usernames</button>
                                        </div>
                                    </div>
                                </form>
                            </div>
                        </div>
                    </div>
                    <div class="col-md-offset-2 col-md-8 col-lg-offset-0 col-lg-6">
                        <div class="panel">
                            <div class="panel-heading">
                                <h2 class="panel-title">Ban IP Addresses from Registering (or logging in)</h2>
                            </div>
                            <div class="panel-body">
                                <form class="form-horizontal" id="security-form-validation3" role="form" action="includes/adminprocess.php" method="POST"> 
                                    <div class="form-group">
                                        <label for="ip_address" class="col-sm-3 control-label">Block / Ban IP Address <span class="text-danger">*</span></label>
                                        <div class="col-sm-6">
                                            <div class="input-group">
                                                <input class="form-control" name="ipaddress"  id="ip_address" placeholder="e.g. 192.168.0.1 without leading zeros">
                                                <span class="input-group-addon"><i class="oi oi-question-mark"></i></span>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="form-group">
                                        <div class="col-sm-offset-3 col-sm-10">
                                            <?php echo $adminfunctions->stopField($session->username, 'configs'); ?>
                                            <input type="hidden" name="form_submission" value="ban_ip">
                                            <button type="submit" class="btn btn-default">Add IP Address</button>
                                        </div>
                                    </div>
                                </form>
                                <form class="form-horizontal" id="security-form-validation4" role="form" action="includes/adminprocess.php" method="POST">
                                    <div class="form-group">
                                        <label for="banned_ip" class="col-sm-3 control-label">Banned IP Addresses </label>
                                        <div class="col-sm-6">
                                        <select name="ipaddress" id="ipaddress" class="form-control" multiple="multiple">
                                        <?php
                                        $sql = "SELECT ban_ip FROM banlist WHERE ban_ip != ''";
                                        $result = $db->prepare($sql);
                                        $result->execute();
                                        while ($row = $result->fetch()) {
                                            $ipaddress = $row['ban_ip'];
                                            echo "<option value='$ipaddress'>$ipaddress</option>";
                                        }
                                        ?>
                                        </select> 
                                        </div>
                                    </div>
                                    <div class="form-group">
                                        <div class="col-sm-offset-3 col-sm-10">
                                            <?php echo $adminfunctions->stopField($session->username, 'configs'); ?>
                                            <input type="hidden" name="form_submission" value="unban_ip">
                                            <button type="submit" class="btn btn-default">Remove Banned IP Addresses</button>
                                        </div>
                                    </div>
                                </form>
                            </div>
                        </div>
                    </div>                                     
                </div>
                <!-- END Row -->

            
                <footer>Copyright &copy; <?php echo date("Y"); ?> <a href="http://ipm.ir" target="_blank">IPM</a> - All rights reserved. </footer>

            </div>
            <!-- END Page Content -->

            <?php include('rightsidebar.php'); ?>

        </div>
        <!-- END Page Wrapper -->
        
        <!-- Scroll to top -->
        <a href="#" id="to-top" class="to-top"><i class="oi oi-chevron-top"></i></a>

        <!-- JavaScript Resources -->
        <script src="js/jquery-2.1.3.min.js"></script>
        <script src="js/bootstrap.min.js"></script>
        <script src="js/plugins/metisMenu/jquery.metisMenu.js"></script>
        <script src="js/komeil.js"></script>
        
        <!-- Initialize Form Validation -->
        <script src="js/plugins/formValidation/securityFormsValidation.js"></script>
        <script src="js/plugins/formValidation/jquery.validate.js"></script>
        <script>$(function() { FormsValidation.init(); });</script>

    </body>
</html>
<?php
}
?>