<?php 
include("includes/controller.php");
$pagename = 'help';
$container = '';
if(!$session->isAdmin()){
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
                    <h2>Help / Support</h2>
                    <ol class="breadcrumb">
                        <li>
                            <a href="index.php">Home</a>
                        </li>
                        <li class="active">
                            Help / Support
                        </li>
                    </ol>
                </div>
                <!-- END Title Header -->
             
                <div class="row">                                     
                    <div class="col-sm-8 col-md-9">
                        <div class="panel">
                            <div class="panel-body">
                                <h4><strong>Xavier PHP Login Script & User Management</strong></h4>
                                Designed and Coded by <a href="http://www.angry-frog.com" target="_blank">Angry Frog</a> using the Xavier <a href="http://www.angry-frog.com/xavier-responsive-admin-theme/" target="_blank">Admin Theme</a> template.<br><br>
                                Support for the script can be found in a number of places including the documentation. If you uploaded the documentation folder along with your script, you can find it <a href='../documentation/index.html'>here</a>.<br><br>
                                Or visit the Angry Frog website where there is lots of information including a message board <a href="http://www.angry-frog.com">here</a>.<br><br>
                                If you need to contact the author about an issue with the script, you can use the Comments section on the Envato website <a href="http://codecanyon.net/item/angry-frog-php-login-script/9146226" target="_blank">here</a>.
                                <h4><strong>Disclaimer</strong></h4>
                                This script is intended for general use and no warranty is implied for suitability to any given task. I hold no responsibility for your setup or any damage done while using/installing/modifying this script or any of its plugins. 
                            </div>
                        </div>
                    </div>
                    <div class="col-sm-4 col-md-3">
                        <div class="panel">
                            <div class="panel-body">
                                <h4><strong>Stats</strong></h4>
                                <?php 
                                echo "<br>\nVersion: ".$configs->getConfig('Version')."<br>\n<br>\n";
                                $result = $db->query('select version()')->fetchColumn();
                                echo "MySQL Version : ".$result."<br>\n";
                                echo "PHP Version : ".phpversion()."<br>\n<br>\n";
                                ?>
                                Changelog : <a href="changelog.txt" target="_blank">Changelog</a>
                            </div>
                        </div>
                    </div>
                </div>

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
        <script src="js/xavier.js"></script>

    </body>
</html>
<?php
}
?>