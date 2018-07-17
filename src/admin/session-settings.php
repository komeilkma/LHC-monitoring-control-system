<?php 
include("includes/controller.php");
$pagename = 'session-settings';
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
        
        <!-- Awesome Bootstrap Checkboxes CSS -->
        <link href="css/plugins/awesome-bootstrap-checkbox/awesome-bootstrap-checkbox.css" rel="stylesheet">      
        
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
                    <h2>Session Settings</h2>
                    <ol class="breadcrumb">
                        <li>
                            <a href="index.php">Home</a>
                        </li>
                        <li class="active">
                            Session Settings
                        </li>
                    </ol>
                </div>
                <!-- END Title Header -->
                
                <div class="row">                                     
                    <div class="col-sm-12 col-md-12">
                        <div class="panel">
                            <div class="panel-body">
                                <h4><strong>Session Settings</strong> - Change the settings regarding sessions.</h4>
                            </div>
                        </div>
                    </div>                                     
                </div>
             
                <div class="row"> 
                    <div class="col-sm-12 col-md-7 col-lg-8">
                        <div class="panel">
                            <div class="panel-heading">
                                <h2 class="panel-title">Session Settings</h2>
                            </div>
                            <div class="panel-body">
                                <form class="form-horizontal" id="session-form-validation" action="includes/adminprocess.php" method="POST"> 
                                    <div class="form-group">
                                        <label for="user_timeout" class="col-sm-4 control-label">User Inactivity Timeout <span class="text-danger">*</span></label>
                                        <div class="col-sm-6 col-lg-4">
                                            <div class="input-group">
                                                <input class="form-control" name="user_timeout" id="user_timeout" placeholder="Required Field.." value="<?php echo $configs->getConfig('USER_TIMEOUT'); ?>">
                                                <span class="input-group-addon">Minutes</span>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="form-group">
                                        <label for="user_timeout" class="col-sm-4 control-label">Guest Timeout <span class="text-danger">*</span></label>
                                        <div class="col-sm-6 col-lg-4">
                                            <div class="input-group">
                                                <input class="form-control" name="guest_timeout" id="guest_timeout" placeholder="Required Field.." value="<?php echo $configs->getConfig('GUEST_TIMEOUT'); ?>">
                                                <span class="input-group-addon">Minutes</span>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="form-group">
                                        <label for="cookie_expiry" class="col-sm-4 control-label">Cookie Expiry <span class="text-danger">*</span></label>
                                        <div class="col-sm-6 col-lg-4">
                                            <div class="input-group">
                                                <input class="form-control" name="cookie_expiry" id="cookie_expiry" placeholder="Required Field.." value="<?php echo $configs->getConfig('COOKIE_EXPIRE'); ?>">
                                                <span class="input-group-addon">Days</span>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="form-group">
                                        <label for="cookie_path" class="col-sm-4 control-label">Cookie Path <span class="text-danger">*</span></label>
                                        <div class="col-sm-6 col-lg-4">
                                            <div class="input-group">
                                                <input class="form-control" name="cookie_path" id="cookie_path" placeholder="Required Field.." value="<?php echo $configs->getConfig('COOKIE_PATH'); ?>">
                                                <span class="input-group-addon"><i class="oi oi-signpost"></i></span>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="form-group">
                                        <div class="col-sm-offset-4 col-sm-8">
                                            <?php echo $adminfunctions->stopField($session->username, 'session'); ?>
                                            <input type="hidden" name="form_submission" value="session_edit">
                                            <button type="submit" class="btn btn-default">Submit</button>
                                        </div>
                                    </div>
                                </form>
                            </div>
                        </div>                    
                    </div>
                    <div class="col-sm-12 col-md-5 col-lg-4">
                        <div class="panel">
                            <div class="panel-heading">
                                <h2 class="panel-title">Need Help ?</h2>
                            </div>
                            <div class="panel-body">
                                <strong>User Inactivity Timeout</strong> - The user is logged out after the set period of inactivity. The default PHP session timeout is usually already set at 24 minutes.<br><br>
                                <strong>Guest Timeout</strong> - A guest is no longer considered a guest (and counted in the whose online figures) after this set period of inactivity.<br><br> 
                                <strong>Cookie Expiry</strong> - This is the amount of days in which the remember me cookie expires.<br><br>
                                <strong>Cookie Path</strong> - The Path attribute defines the scope of the cookie. Leave as <strong>/</strong> by default.
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
        <script src="js/plugins/formValidation/sessionFormsValidation.js"></script>
        <script src="js/plugins/formValidation/jquery.validate.js"></script>
        <script>$(function() { FormsValidation.init(); });</script>       

    </body>
</html>
<?php
}
?>